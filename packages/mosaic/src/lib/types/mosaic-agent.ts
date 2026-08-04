export interface MosaicAgent {
  prompt(input: string): Promise<void>;
}
