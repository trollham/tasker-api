-- Add up migration script here
ALTER TABLE tasks ADD COLUMN state TEXT NOT NULL DEFAULT 'incomplete';
