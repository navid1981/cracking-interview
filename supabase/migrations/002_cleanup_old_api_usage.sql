-- Enable pg_cron extension (Supabase has this pre-installed)
CREATE EXTENSION IF NOT EXISTS pg_cron;

-- Grant usage to postgres user (required for scheduling)
GRANT USAGE ON SCHEMA cron TO postgres;

-- Create a function to delete old api_usage records
CREATE OR REPLACE FUNCTION cleanup_old_api_usage()
RETURNS void
LANGUAGE plpgsql
SECURITY DEFINER
AS $$
DECLARE
  deleted_count INTEGER;
BEGIN
  DELETE FROM public.api_usage
  WHERE created_at < NOW() - INTERVAL '3 months';
  
  GET DIAGNOSTICS deleted_count = ROW_COUNT;
  
  RAISE NOTICE 'Cleaned up % old api_usage records', deleted_count;
END;
$$;

-- Schedule the cleanup job to run daily at 3:00 AM UTC
-- First, remove existing job if it exists (to make this idempotent)
SELECT cron.unschedule('cleanup-old-api-usage')
WHERE EXISTS (
  SELECT 1 FROM cron.job WHERE jobname = 'cleanup-old-api-usage'
);

-- Schedule the new job
SELECT cron.schedule(
  'cleanup-old-api-usage',           -- job name
  '0 3 * * *',                       -- cron expression: daily at 3:00 AM UTC
  $$SELECT cleanup_old_api_usage()$$ -- SQL to execute
);

-- Verify the job was created
-- You can check scheduled jobs with: SELECT * FROM cron.job;

