import axios from 'axios'

export async function sleep(delay) {
  return new Promise((resolve) => setTimeout(resolve, delay))
}

export async function fetchRetry(url, delay, tries) {
  function onError(err) {
    const triesLeft = tries - 1
    console.log(`Attempting to fetch ${url}. Tries left: ${triesLeft}...`)
    if (!triesLeft) {
      throw err
    }
    return sleep(delay).then(() => fetchRetry(url, delay, triesLeft))
  }
  return axios.get(url).catch(onError)
}
