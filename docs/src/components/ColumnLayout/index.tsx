import React from 'react'

import styles from './columns.module.css'

export const TwoColumns = ({ children }: { children: React.ReactNode[] }) => (
  <div className={styles.columnsWrapper}>
    <div className={styles.column}>{children[0]}</div>
    <div className={styles.column}>{children[1]}</div>
  </div>
)
