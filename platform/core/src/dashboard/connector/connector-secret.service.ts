import { Injectable } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'

import * as crypto from 'node:crypto'

@Injectable()
export class ConnectorSecretService {
  constructor(private readonly configService: ConfigService) {}

  encrypt(value: unknown): string {
    const key = this.key()
    const iv = crypto.randomBytes(12)
    const cipher = crypto.createCipheriv('aes-256-gcm', key, iv)
    const body = Buffer.concat([cipher.update(JSON.stringify(value), 'utf8'), cipher.final()])
    const tag = cipher.getAuthTag()
    return [iv.toString('base64'), tag.toString('base64'), body.toString('base64')].join('.')
  }

  decrypt<T = unknown>(ciphertext: string): T {
    const [ivRaw, tagRaw, bodyRaw] = ciphertext.split('.')
    const decipher = crypto.createDecipheriv('aes-256-gcm', this.key(), Buffer.from(ivRaw, 'base64'))
    decipher.setAuthTag(Buffer.from(tagRaw, 'base64'))
    const body = Buffer.concat([decipher.update(Buffer.from(bodyRaw, 'base64')), decipher.final()]).toString(
      'utf8'
    )
    return JSON.parse(body) as T
  }

  private key(): Buffer {
    const configured = this.configService.get<string>('RUSHDB_AES_256_ENCRYPTION_KEY') ?? ''
    return crypto
      .createHash('sha256')
      .update(configured || 'rushdb-local-connector-key')
      .digest()
  }
}
