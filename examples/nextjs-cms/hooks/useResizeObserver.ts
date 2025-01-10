'use client';

import {
  useEffect,
  useRef,
  useCallback,
  useState,
  MutableRefObject,
} from 'react';

export interface IObservableSize {
  readonly width: number;
  readonly height: number;
}

export const useResizeObserver = <T extends Element>({
  onResize,
}: {
  onResize(size: IObservableSize): void;
}): MutableRefObject<T | null> => {
  const ref = useRef<T | null>(null);
  const resizeRef = useRef(onResize);
  resizeRef.current = onResize;
  const previous = useRef({
    width: 0,
    height: 0,
  });
  useEffect(() => {
    if (
      typeof ref !== 'object' ||
      ref === null ||
      !(ref.current instanceof Element)
    ) {
      return;
    }
    const element = ref.current;
    const resizeObserver = new ResizeObserver((entries) => {
      if (!Array.isArray(entries)) {
        return;
      }
      if (!entries.length) {
        return;
      }
      const entry = entries[0];
      const width = Math.round(entry.contentRect.width);
      const height = Math.round(entry.contentRect.height);
      if (
        previous.current.width !== width ||
        previous.current.height !== height
      ) {
        previous.current.width = width;
        previous.current.height = height;
        resizeRef.current({ width, height });
      }
    });
    resizeObserver.observe(element);
    return () => resizeObserver.unobserve(element);
  }, []);

  return ref;
};

export const useObservedSize = <T extends Element>(
  debounceTimeout: number = 0
): { ref: MutableRefObject<T | null>; size: IObservableSize } => {
  const [size, setSize] = useState<IObservableSize>({ width: 0, height: 0 });
  const timeoutRef = useRef<number | undefined>();
  const clear = useCallback(() => {
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current);
    }
  }, [timeoutRef]);
  const onResize = useCallback(
    (newSize: IObservableSize) => {
      if (debounceTimeout) {
        clear();
        timeoutRef.current = window.setTimeout(() => {
          setSize(newSize);
        }, debounceTimeout);
      } else {
        setSize(newSize);
      }
    },
    [timeoutRef, debounceTimeout]
  );
  useEffect(() => {
    return clear;
  }, [clear]);

  const ref = useResizeObserver<T>({ onResize });
  return { ref, size };
};
