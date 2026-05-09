// Extract invite token from URL
export function getInviteTokenFromURL(): string | null {
  const url = new URL(window.location.href)
  // Check for both possible query parameter names
  return url.searchParams.get('invite') || url.searchParams.get('token')
}
