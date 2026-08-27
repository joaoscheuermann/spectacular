export const planningCase = {
  name: 'production migration',
  objective:
    'Deploy the new billing service version to production without interrupting active checkouts. The release includes a database migration.',
  skills: [
    {
      name: 'safe-database-release',
      body: `# Safe database release

Treat schema compatibility, application rollout, health verification, and rollback as one release decision. The migration must remain compatible with both application versions during rollout, and rollback must be possible before removing the old schema.`,
    },
  ],
};
