import { Injectable } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { hash, compare } from 'bcryptjs'

const HASH_ROUNDS = 10

@Injectable()
export class EncryptionService {
  constructor(private readonly config: ConfigService) {}

  async hash(plain: string): Promise<string> {
    return hash(plain, HASH_ROUNDS)
  }

  async compare(plain: string, encrypted: string): Promise<boolean> {
    return compare(plain, encrypted)
  }
}
