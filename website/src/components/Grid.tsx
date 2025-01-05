import classNames from "classnames"
import {
  CSSProperties,
  Children,
  ComponentPropsWithoutRef,
  createContext,
  useContext,
  useMemo,
} from "react"
import { useMediaQuery } from "~/hooks/useMediaQuery"

// Context to manage grid styling and behavior
const GridContext = createContext<{
  columns: number
  rows: number
  totalItems: number
}>({
  columns: 0,
  rows: 0,
  totalItems: 0,
})

export const useGridContext = () => useContext(GridContext)

export function GridItem({
  idx,
  className,
  // firstRow: firstRowClassName = "",
  // lastRow: lastRowClassName = "",
  // middleRow: middleRowClassName = "",
  lastOfFirstRow: lastOfFirstRowClassName = "",
  firstOfLastRow: firstOfLastRowClassName = "",
  firstOfMiddleRow: firstOfMiddleRowClassName = "",
  lastOfMiddleRow: lastOfMiddleRowClassName = "",
  middleOfFirstRow = "",
  middleOfLastRow = "",
  ...props
}: {
  idx: number
  // firstRow?: string;
  // lastRow?: string;
  // middleRow?: string;
  firstOfFirstRow?: string
  lastOfFirstRow?: string
  firstOfMiddleRow?: string
  lastOfMiddleRow?: string
  firstOfLastRow?: string
  lastOfLastRow?: string
  middleOfFirstRow?: string
  middleOfLastRow?: string
} & ComponentPropsWithoutRef<"article">) {
  const { rows, columns, totalItems } = useGridContext()

  const firstOfLastRow = (rows - 1) * columns
  const firstRow = idx < columns - 1
  const lastRow = idx > firstOfLastRow
  const middleRow = !firstRow && !lastRow
  const firstOfRow = idx % columns === 0
  const lastOfRow = idx % columns === columns - 1

  return (
    <article
      className={classNames(
        {
          // [firstRowClassName]: firstRow,
          // [middleRowClassName]: middleRow,
          // [lastRowClassName]: lastRow,
          [lastOfFirstRowClassName]: idx === columns - 1,
          [firstOfLastRowClassName]: idx === firstOfLastRow,
          [firstOfMiddleRowClassName]: middleRow && firstOfRow,
          [lastOfMiddleRowClassName]: middleRow && lastOfRow,
          [middleOfFirstRow]: firstRow && !firstOfRow && !lastOfRow,
          [middleOfLastRow]: lastRow && !firstOfRow && !lastOfRow,
        },
        className,
      )}
      {...props}
    />
  )
}

export function Grid({
  desktopCols,
  tabletCols,
  mobileCols,
  className,
  children,
  ...props
}: {
  tabletCols: number
  mobileCols: number
  desktopCols: number
} & ComponentPropsWithoutRef<"div">) {
  const desktop = useMediaQuery("(min-width: 1200px)")
  const tablet = useMediaQuery("(min-width: 768px)")

  const totalItems = Children.count(children)
  const columns = desktop ? desktopCols : tablet ? tabletCols : mobileCols
  const rows = Math.ceil(totalItems / columns)

  return (
    <GridContext.Provider
      value={useMemo(
        () => ({ columns, rows, totalItems }),
        [columns, rows, totalItems],
      )}
    >
      <div
        className={classNames(
          "grid grid-cols-[repeat(var(--columns),_minmax(0,_1fr))]",
          className,
        )}
        style={
          {
            "--columns": columns,
          } as CSSProperties
        }
        {...props}
      >
        {children}
      </div>
    </GridContext.Provider>
  )
}
