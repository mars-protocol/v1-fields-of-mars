/**
 * Encode a JSON object to base64 string
 */
export function encodeBase64(obj: object | string | number) {
  return Buffer.from(JSON.stringify(obj)).toString("base64");
}

/**
 * Encode a string to UTF8 array
 */
export function encodeUtf8(str: string) {
  const encoder = new TextEncoder();
  return Array.from(encoder.encode(str));
}
