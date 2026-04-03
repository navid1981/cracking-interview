use serde::{Serialize, Deserialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentPlaceholder {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub file_name: String,
    pub extracted_text: String,
    pub created_at: String,
}

const MAX_TEXT_LENGTH: usize = 50_000;

pub fn extract_text_from_bytes(file_name: &str, data: &[u8]) -> Result<String, String> {
    let ext = file_name.rsplit('.').next().unwrap_or("").to_lowercase();
    let text = match ext.as_str() {
        "txt" | "text" | "md" => extract_txt(data)?,
        "pdf" => extract_pdf(data)?,
        "docx" => extract_docx(data)?,
        "doc" => extract_doc(data)?,
        _ => return Err(format!("Unsupported file type: .{}. Supported: PDF, DOCX, DOC, TXT", ext)),
    };

    let trimmed = text.trim().to_string();
    if trimmed.is_empty() {
        return Err("No text could be extracted from this file.".to_string());
    }

    if trimmed.len() > MAX_TEXT_LENGTH {
        Ok(trimmed[..MAX_TEXT_LENGTH].to_string())
    } else {
        Ok(trimmed)
    }
}

fn extract_txt(data: &[u8]) -> Result<String, String> {
    String::from_utf8(data.to_vec())
        .map_err(|_| "File is not valid UTF-8 text.".to_string())
}

fn extract_pdf(data: &[u8]) -> Result<String, String> {
    pdf_extract::extract_text_from_mem(data)
        .map_err(|e| format!("Failed to extract PDF text: {}", e))
}

fn extract_docx(data: &[u8]) -> Result<String, String> {
    use std::io::{Cursor, Read};

    let reader = Cursor::new(data);
    let mut archive = zip::ZipArchive::new(reader)
        .map_err(|e| format!("Not a valid DOCX file: {}", e))?;

    let mut xml_content = String::new();
    let mut file = archive.by_name("word/document.xml")
        .map_err(|_| "Not a valid DOCX file (missing word/document.xml)".to_string())?;
    file.read_to_string(&mut xml_content)
        .map_err(|e| format!("Failed to read document.xml: {}", e))?;

    extract_text_from_ooxml(&xml_content)
}

fn extract_text_from_ooxml(xml: &str) -> Result<String, String> {
    use quick_xml::Reader;
    use quick_xml::events::Event;

    let mut reader = Reader::from_str(xml);
    let mut text = String::new();
    let mut in_t_tag = false;
    let mut last_was_paragraph = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let local = e.local_name();
                match local.as_ref() {
                    b"t" => in_t_tag = true,
                    b"p" => {
                        if last_was_paragraph && !text.is_empty() {
                            text.push('\n');
                        }
                        last_was_paragraph = true;
                    }
                    b"br" => text.push('\n'),
                    b"tab" => text.push('\t'),
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                if e.local_name().as_ref() == b"t" {
                    in_t_tag = false;
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_t_tag {
                    if let Ok(t) = e.unescape() {
                        text.push_str(&t);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML parse error: {}", e)),
            _ => {}
        }
    }

    Ok(text)
}

fn extract_doc(data: &[u8]) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        let tmp_path = std::env::temp_dir().join("cracking_interview_doc_extract.doc");
        std::fs::write(&tmp_path, data)
            .map_err(|e| format!("Failed to write temp file: {}", e))?;

        let output = std::process::Command::new("textutil")
            .args(["-convert", "txt", "-stdout"])
            .arg(&tmp_path)
            .output()
            .map_err(|e| format!("textutil not available: {}", e))?;

        let _ = std::fs::remove_file(&tmp_path);

        if output.status.success() {
            String::from_utf8(output.stdout)
                .map_err(|_| "Failed to decode textutil output".to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("textutil failed: {}", stderr))
        }
    }

    #[cfg(target_os = "windows")]
    {
        let _ = data;
        Err("Legacy .doc format is not supported on Windows. Please save your document as .docx and try again.".to_string())
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = data;
        Err("Legacy .doc format is not supported on this platform. Please save your document as .docx and try again.".to_string())
    }
}

pub fn documents_config_path(config_dir: &PathBuf) -> PathBuf {
    config_dir.join("documents.json")
}

pub fn load_placeholders(config_dir: &PathBuf) -> Vec<DocumentPlaceholder> {
    let path = documents_config_path(config_dir);
    if !path.exists() {
        return Vec::new();
    }
    match std::fs::read_to_string(&path) {
        Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

pub fn save_placeholders(config_dir: &PathBuf, placeholders: &[DocumentPlaceholder]) -> Result<(), String> {
    let path = documents_config_path(config_dir);
    let json = serde_json::to_string_pretty(placeholders)
        .map_err(|e| format!("Failed to serialize: {}", e))?;
    std::fs::write(path, json)
        .map_err(|e| format!("Failed to write documents.json: {}", e))?;
    Ok(())
}
