export type OAuthHttpMethod = 'GET' | 'POST';

export type OAuthHttpHeaders = Readonly<Record<string, string>>;

export type OAuthHttpRequest = {
  readonly method: OAuthHttpMethod;
  readonly url: string;
  readonly headers?: OAuthHttpHeaders;
  readonly body?: string;
  readonly signal?: AbortSignal;
};

export type OAuthHttpResponse = {
  readonly status: number;
  readonly headers: OAuthHttpHeaders;
  readonly body: string;
};

/** Boundary for OAuth HTTP calls; callers and tests inject the concrete transport. */
export interface OAuthTransport {
  request(request: OAuthHttpRequest): Promise<OAuthHttpResponse>;
}
