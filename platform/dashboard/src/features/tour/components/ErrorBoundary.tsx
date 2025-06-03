import React from 'react'

interface Props {
  children: React.ReactNode
}
interface State {
  hasError: boolean
}

export class ErrorBoundary extends React.Component<Props, State> {
  constructor(props: Props) {
    super(props)
    this.state = { hasError: false }
  }

  static getDerivedStateFromError(error: any) {
    return { hasError: true }
  }

  componentDidCatch(error: any, info: any) {
    console.error('ErrorBoundary caught Joyride error:', error, info)
  }

  render() {
    return this.props.children
  }
}
