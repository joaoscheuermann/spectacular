import { PrismaPg } from '@prisma/adapter-pg';

import { PrismaClient } from '../generated/prisma/client.js';

export type Database = PrismaClient;

/** Creates the process-owned Prisma client over PostgreSQL's native pool. */
export const createDatabase = (connectionString: string): Database => {
  if (connectionString.trim().length === 0) {
    throw new Error('DORIC_DATABASE_URL is required.');
  }

  return new PrismaClient({
    adapter: new PrismaPg({ connectionString }),
  });
};
