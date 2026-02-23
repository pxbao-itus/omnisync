-- Add user info columns to credentials table
ALTER TABLE credentials ADD COLUMN user_name TEXT;
ALTER TABLE credentials ADD COLUMN user_email TEXT;
ALTER TABLE credentials ADD COLUMN user_avatar TEXT;
