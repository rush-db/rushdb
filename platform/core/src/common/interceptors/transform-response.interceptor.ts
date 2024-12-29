import { Injectable, NestInterceptor, ExecutionContext, CallHandler } from '@nestjs/common'
import { Observable } from 'rxjs'
import { map } from 'rxjs/operators'

export interface IResponse<T> {
  data: T
}

@Injectable()
export class TransformResponseInterceptor<T> implements NestInterceptor<T, IResponse<T>> {
  intercept(context: ExecutionContext, next: CallHandler): Observable<IResponse<T>> {
    return next.handle().pipe(
      map((data) => ({
        data: data && data.data !== undefined ? data.data : data,
        total: data && data.total !== undefined ? data.total : undefined,
        success: true
      }))
    )
  }
}
