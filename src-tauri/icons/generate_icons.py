#!/usr/bin/env python3
"""Generate placeholder icons for Phase 1"""

from PIL import Image, ImageDraw

def create_icon(size, filename):
    """Create a simple icon with CI letters"""
    img = Image.new('RGB', (size, size), color='#3380E6')
    draw = ImageDraw.Draw(img)
    
    # Draw a circle
    margin = size // 4
    draw.ellipse([margin, margin, size - margin, size - margin], fill='#4DCCCC80')
    
    img.save(filename)
    print(f"✅ Created {filename}")

print("Creating CrackingInterview icons...")

create_icon(32, '32x32.png')
create_icon(128, '128x128.png')
create_icon(128, '128x128@2x.png')
create_icon(256, 'icon.png')
create_icon(512, 'icon.icns')

# Windows icons
create_icon(44, 'Square44x44Logo.png')
create_icon(50, 'StoreLogo.png')
create_icon(89, 'Square89x89Logo.png')
create_icon(310, 'Square310x310Logo.png')

print("\n🎉 All icons created!")
