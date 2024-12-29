import { Injectable } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { hash, compare } from 'bcryptjs'

@Injectable()
export class EncryptionService {
  constructor(private readonly config: ConfigService) {}

  async hash(plain: string): Promise<string> {
    return hash(plain, parseInt(this.config.get('HASH_ROUNDS', '10'), 10))
  }

  async compare(plain: string, encrypted: string): Promise<boolean> {
    return compare(plain, encrypted)
  }
}
