/** Links that may be handed to the OS default browser: http and https only. */
export function isExternalHttpUrl(href: string): boolean {
  try {
    const url = new URL(href);
    return url.protocol === "http:" || url.protocol === "https:";
  } catch {
    return false;
  }
}
