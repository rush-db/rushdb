import { execFile } from "child_process";
import { platform } from "os";
import { URL } from "url";

export async function OpenBrowser(params: { url: string }) {
  const { url } = params;
  
  // Validate URL to prevent command injection
  let validatedUrl: string;
  try {
    // Validate URL format
    const parsedUrl = new URL(url);
    // Only allow http and https protocols
    if (!['http:', 'https:'].includes(parsedUrl.protocol)) {
      return {
        success: false,
        message: "Invalid URL protocol. Only http and https are supported.",
      };
    }
    validatedUrl = parsedUrl.toString();
  } catch (e) {
    return {
      success: false,
      message: `Invalid URL format: ${e instanceof Error ? e.message : String(e)}`,
    };
  }
  
  // Determine the command based on platform
  const command =
    platform() === "darwin"
      ? "open"
      : platform() === "win32"
      ? "start"
      : "xdg-open";

  return new Promise<{ success: boolean; message: string }>(
    (resolve) => {
      // Use execFile instead of exec to prevent command injection
      execFile(command, [validatedUrl], (error) => {
        if (error) {
          resolve({
            success: false,
            message: `Failed to open browser: ${error.message}`,
          });
        } else {
          resolve({
            success: true,
            message: `Successfully opened ${validatedUrl} in default browser`,
          });
        }
      });
    }
  );
}
