declare module "*.html" {
  const value: any
  export default value
}

declare global {
  // A more precise version of just React.ComponentPropsWithoutRef on its own
  type TPropsOf<
    As extends
      | React.JSXElementConstructor<unknown>
      | keyof JSX.IntrinsicElements,
  > = JSX.LibraryManagedAttributes<As, React.ComponentPropsWithoutRef<As>>

  type AsProp<As extends React.ElementType> = {
    /**
     * An override of the default HTML tag.
     * Can also be another React component.
     */
    as?: As
  }

  /**
   * Allows for extending a set of props (`ExtendedProps`) by an overriding set of props
   * (`OverrideProps`), ensuring that any duplicates are overridden by the overriding
   * set of props.
   */
  type TExtendableProps<
    ExtendedProps = IObject,
    OverrideProps = IObject,
  > = OverrideProps & Omit<ExtendedProps, keyof OverrideProps>

  /**
   * Allows for inheriting the props from the specified element type so that
   * props like children, className & style work, as well as element-specific
   * attributes like aria roles. The component (`C`) must be passed in.
   */
  type TInheritableElementProps<
    As extends React.ElementType,
    Props = IObject,
  > = TExtendableProps<TPropsOf<As>, Props>

  /**
   * A more sophisticated version of `TInheritableElementProps` where
   * the passed in `as` prop will determine which props can be included
   */
  type TPolymorphicComponentProps<
    As extends React.ElementType,
    Props = {},
  > = TInheritableElementProps<As, Props & AsProp<As>>

  /**
   * Utility type to extract the `ref` prop from a polymorphic component
   */
  type TPolymorphicRef<As extends React.ElementType> =
    React.ComponentPropsWithRef<As>["ref"]

  /**
   * A wrapper of `TPolymorphicComponentProps` that also includes the `ref`
   * prop for the polymorphic component
   */
  type TPolymorphicComponentPropsWithRef<
    As extends React.ElementType,
    Props = IObject,
  > = TPolymorphicComponentProps<As, Props> & {
    ref?: TPolymorphicRef<As>
  }

  /**
   * Extends component props with props from `as` attribute
   *
   * @example
   * ```typescript
   * const Button: TPolymorphicComponent<
   *  TButtonProps, // <- component props type
   *  "button" // <- default render tag
   * > = React.forwardRef((props, ref) => {
   *   return <StyledButton {...props} ref={ref} />;
   * });
   * ```
   */
  type TPolymorphicComponent<
    ComponentProps extends IObject = IObject,
    DefaultElementType extends React.ElementType = "div",
  > = {
    <As extends React.ElementType = DefaultElementType>(
      props: TPolymorphicComponentPropsWithRef<As, ComponentProps>,
      ref?: TPolymorphicRef<As>,
    ): React.ReactElement | null
    displayName?: string | undefined
  }
}

export {}
