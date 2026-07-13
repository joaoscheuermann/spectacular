const TOKENS =
  /\/\/[^\r\n]*|\/\*[\s\S]*?(?:\*\/|(?![\s\S]))|"(?:\\[^\r\n]|[^"\\\r\n])*"|'(?:\\[^\r\n]|[^'\\\r\n])*'|(^[ \t]*|(?:=>|[=(:,])[ \t]*|(?<![.$#\p{ID_Continue}])(?:return|case|throw|yield)[ \t]+)(\/(?![/*])(?:\\[^\r\n]|\[(?:\\[^\r\n]|[^\]\\\r\n])*\]|[^/\\[\r\n])+\/(?![dgimsuvy]*([dgimsuvy])[dgimsuvy]*\3)(?![dgimsuvy]*(?:u[dgimsuvy]*v|v[dgimsuvy]*u))[dgimsuvy]*(?![$\p{ID_Continue}]))/gmu;

/** Redacts context-gated regex literals; template strings and full lexical parsing remain outside this POC. */
export const redactRegex = (body: string): string =>
  body.replace(
    TOKENS,
    (token, context: string | undefined, literal: string | undefined) =>
      literal ? `${context}[regex redacted]` : token,
  );
