import { atom } from 'nanostores'

const initialInvite: string | null = null

export const $inviteToken = atom<string | null>(initialInvite)
