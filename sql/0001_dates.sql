ALTER TABLE games add column start_date DATETIME default '1970-01-01 00:00:00';
ALTER TABLE games add column end_date DATETIME default '1970-01-01 00:00:00';
ALTER TABLE moves add column move_date DATETIME default '1970-01-01 00:00:00';
