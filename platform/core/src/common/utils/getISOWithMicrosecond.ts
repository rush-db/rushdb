export const getISOWithMicrosecond = () => {
  let [, n] = process.hrtime()
  const s = new Date().getTime() / 1000

  return (
    // eslint-disable-next-line @typescript-eslint/ban-ts-comment
    // @ts-ignore
    new Date(s * 1e3).toJSON((n = 0 | (n / 1e3))).slice(0, 20 - !n) + `${n + 1e6}`.slice(n ? 1 : 7)
  )
}
