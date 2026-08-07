import type { FinalDelivery } from 'mosaic';

type Writer = Pick<NodeJS.WriteStream, 'write'>;

/** Writes only final Markdown, ending the stream with exactly one newline. */
export const writeDelivery = (
  delivery: FinalDelivery,
  writer: Writer,
): void => {
  const { markdown } = delivery;
  writer.write(markdown.endsWith('\n') ? markdown : `${markdown}\n`);
};
