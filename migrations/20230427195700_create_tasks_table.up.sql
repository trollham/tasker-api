-- Add up migration script here

CREATE TABLE tasks (
	id UUID NOT NULL PRIMARY KEY,
	-- using text isn't the most performant, but if more systems are built on top of
	-- this database we won't need to maintain strict versioning around a `task_type`
	-- enum. Of course, this could also be resolved by using a foreign key pointing 
	-- to a task_type definition table, but that feels overkill for this
	task_type text NOT NULL,
	submitted timestamptz NOT NULL
);

CREATE INDEX IF NOT EXISTS INDEX_task_type ON tasks (task_type);

