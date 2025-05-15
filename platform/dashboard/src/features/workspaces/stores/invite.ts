import { atom } from 'nanostores'

let initialInvite: string | null = null

export const $inviteToken = atom<string | null>(initialInvite)
