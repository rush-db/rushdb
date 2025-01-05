import React from 'react'

export const Method = ({ method, color }: { method: string; color: string }) => (
  <span style={{ backgroundColor: color, color: 'white', padding: '2px 6px', borderRadius: '4px' }}>
    {method}
  </span>
)
