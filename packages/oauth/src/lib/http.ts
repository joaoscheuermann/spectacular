import type { OAuthHttpRequest, OAuthHttpResponse, OAuthTransport } from './types/http.js';

const headersToRecord = (headers: Headers): Record<string, string> =>
  Object.fromEntries(headers.entries());

/** Creates an OAuth transport around Fetch while keeping token calls testable. */
export const createFetchTransport = (
  fetcher: typeof fetch = fetch,
): OAuthTransport => ({
  async request(request: OAuthHttpRequest): Promise<OAuthHttpResponse> {
    const response = await fetcher(request.url, {
      method: request.method,
      headers: request.headers,
      body: request.body,
      signal: request.signal,
    });

    return {
      status: response.status,
      headers: headersToRecord(response.headers),
      body: await response.text(),
    };
  },
});
