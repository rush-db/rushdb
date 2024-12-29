import { Session, Transaction } from 'neo4j-driver'

export type TTransactionObject = {
  id: string
  projectId: string
  startTime: string
  session: Session
  transaction: Transaction
}
