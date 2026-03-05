-- Audio recording usage tracking for Pro users (10 hours/month limit)
CREATE TABLE IF NOT EXISTS public.audio_usage (
  id UUID DEFAULT gen_random_uuid() PRIMARY KEY,
  user_id UUID NOT NULL REFERENCES auth.users(id),
  duration_seconds INTEGER NOT NULL,
  created_at TIMESTAMPTZ DEFAULT now()
);

CREATE INDEX idx_audio_usage_user_period ON public.audio_usage(user_id, created_at);
ALTER TABLE public.audio_usage ENABLE ROW LEVEL SECURITY;

-- Cleanup: delete audio_usage records older than 3 months (same pattern as api_usage)
CREATE OR REPLACE FUNCTION cleanup_old_audio_usage()
RETURNS void
LANGUAGE plpgsql
SECURITY DEFINER
AS $$
DECLARE
  deleted_count INTEGER;
BEGIN
  DELETE FROM public.audio_usage
  WHERE created_at < NOW() - INTERVAL '3 months';

  GET DIAGNOSTICS deleted_count = ROW_COUNT;

  RAISE NOTICE 'Cleaned up % old audio_usage records', deleted_count;
END;
$$;

SELECT cron.unschedule('cleanup-old-audio-usage')
WHERE EXISTS (
  SELECT 1 FROM cron.job WHERE jobname = 'cleanup-old-audio-usage'
);

SELECT cron.schedule(
  'cleanup-old-audio-usage',
  '0 3 * * *',
  $$SELECT cleanup_old_audio_usage()$$
);

-- Helper: sum audio seconds for a user within a billing period
CREATE OR REPLACE FUNCTION public.sum_audio_seconds(
  p_user_id UUID,
  p_start TIMESTAMPTZ,
  p_end TIMESTAMPTZ
)
RETURNS BIGINT
LANGUAGE sql
STABLE
SECURITY DEFINER
AS $$
  SELECT COALESCE(SUM(duration_seconds), 0)
  FROM public.audio_usage
  WHERE user_id = p_user_id
    AND created_at >= p_start
    AND created_at < p_end;
$$;
