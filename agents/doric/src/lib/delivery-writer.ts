import type { MosaicResult } from 'mosaic';

type Writer = Pick<NodeJS.WriteStream, 'write'>;
type Logger = { info(bindings: unknown, message: string): void };

/** Writes completed Markdown or safely logs a non-completed terminal result. */
export const writeResult = (
  result: MosaicResult,
  writer: Writer,
  logger: Logger,
): void => {
  if (result.status !== 'completed') {
    logger.info(
      { status: result.status, nodeIds: result.nodes.map(({ id }) => id) },
      'mosaic workflow did not complete',
    );
    return;
  }

  const { delivery } = result;
  const { markdown } = delivery;
  writer.write(markdown.endsWith('\n') ? markdown : `${markdown}\n`);
};
