import { Session, Transaction } from 'neo4j-driver'

export type TTransactionStatus = 'open' | 'committing' | 'rolling_back' | 'closed' | 'expired'

export type TTransactionObject = {
  id: string
  projectId: string
  startTime: string
  session: Session
  transaction: Transaction
  timeout: NodeJS.Timeout
  status: TTransactionStatus
}
