import axios from 'axios'

export async function sleep(delayMs: number) {
  return new Promise((resolve) => setTimeout(resolve, delayMs))
}

export async function fetchRetry(url: string, delayMs: number, tries: number) {
  function onError(err) {
    const triesLeft = tries - 1
    console.log(`Attempting to fetch ${url}. Tries left: ${triesLeft}...`)
    if (!triesLeft) {
      throw err
    }
    return sleep(delayMs).then(() => fetchRetry(url, delayMs, triesLeft))
  }
  return axios.get(url).catch(onError)
}
