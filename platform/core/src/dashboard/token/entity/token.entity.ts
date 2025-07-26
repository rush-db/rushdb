import { ITokenProperties } from '@/dashboard/token/model/token.interface'

export class TokenEntity {
  constructor(
    private readonly id: string,
    private readonly name: string,
    private readonly created: string,
    private readonly expiration: number,
    private readonly value: string,
    private readonly description?: string,
    private readonly prefixValue?: string
  ) {}

  getProperties(): ITokenProperties & { value: string } {
    return {
      id: this.id,
      name: this.name,
      created: this.created,
      expiration: this.expiration,
      value: this.prefixValue ? `${this.prefixValue}${this.value}` : this.value,
      description: this.description
    }
  }

  toJson() {
    return this.getProperties()
  }
}
