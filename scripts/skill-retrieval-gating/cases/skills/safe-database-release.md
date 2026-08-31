# Safe database release

Coordinates compatible database migrations and application rollouts.

Treat schema compatibility, application rollout, health verification, and rollback as one release decision. The migration must remain compatible with both application versions during rollout, and rollback must be possible before removing the old schema.
