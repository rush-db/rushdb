export function SvgIcon({
  name,
  prefix = 'icon',
  color = 'currentColor',
  width = 24,
  height = 24,
  ...props
}: TPolymorphicComponentProps<
  'svg',
  {
    prefix?: string
  }
>) {
  const symbolId = `#${prefix}-${name}`

  return (
    <svg {...props} aria-hidden="true" height={height} width={width}>
      <use fill={color} href={symbolId} />
    </svg>
  )
}
