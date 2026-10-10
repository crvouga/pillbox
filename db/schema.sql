-- Atlas desired state. This is a declarative schema, not a versioned change script.
CREATE TABLE users (
 id uuid PRIMARY KEY,
 name text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 64),
 created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE sessions (
 token_hash text PRIMARY KEY,
 user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
 expires_at timestamptz NOT NULL
);
CREATE INDEX sessions_user_idx ON sessions(user_id);
CREATE INDEX sessions_expiry_idx ON sessions(expires_at);
CREATE TABLE maps (
 id uuid PRIMARY KEY,
 owner_id uuid NOT NULL REFERENCES users(id),
 created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX maps_owner_idx ON maps(owner_id);
CREATE TABLE map_versions (
 id uuid PRIMARY KEY,
 map_id uuid NOT NULL REFERENCES maps(id),
 version integer NOT NULL CHECK (version > 0),
 document jsonb NOT NULL CHECK (jsonb_typeof(document) = 'object'),
 published boolean NOT NULL DEFAULT false,
 created_at timestamptz NOT NULL DEFAULT now(),
 UNIQUE(map_id, version)
);
CREATE INDEX map_versions_published_idx ON map_versions(published);
CREATE TABLE modes (
 id uuid PRIMARY KEY,
 owner_id uuid NOT NULL REFERENCES users(id),
 created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX modes_owner_idx ON modes(owner_id);
CREATE TABLE mode_versions (
 id uuid PRIMARY KEY,
 mode_id uuid NOT NULL REFERENCES modes(id),
 version integer NOT NULL CHECK (version > 0),
 document jsonb NOT NULL CHECK (jsonb_typeof(document) = 'object'),
 published boolean NOT NULL DEFAULT false,
 created_at timestamptz NOT NULL DEFAULT now(),
 UNIQUE(mode_id, version)
);
CREATE INDEX mode_versions_published_idx ON mode_versions(published);
CREATE TABLE match_results (
 id uuid PRIMARY KEY,
 server_id uuid NOT NULL,
 map_version_id uuid NOT NULL REFERENCES map_versions(id),
 mode_version_id uuid NOT NULL REFERENCES mode_versions(id),
 result jsonb NOT NULL CHECK (jsonb_typeof(result) = 'object'),
 finished_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX match_results_finished_idx ON match_results(finished_at);
CREATE TABLE match_players (
 match_id uuid NOT NULL REFERENCES match_results(id),
 user_id uuid NOT NULL REFERENCES users(id),
 score integer NOT NULL,
 kills integer NOT NULL CHECK (kills >= 0),
 deaths integer NOT NULL CHECK (deaths >= 0),
 team integer NOT NULL CHECK (team IN (0,1)),
 PRIMARY KEY(match_id, user_id)
);
CREATE INDEX match_players_user_idx ON match_players(user_id);
