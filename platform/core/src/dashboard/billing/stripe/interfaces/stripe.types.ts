export type TPlan = {
  [planKey: string]: {
    [periodKey: string]: {
      amount: number
      priceId: string
      productId: string
    }
  }
}
