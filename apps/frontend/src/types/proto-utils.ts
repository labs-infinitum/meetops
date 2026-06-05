/** Replace protobuf `int64` (`bigint`) fields with `number` for app/UI use. */
export type Int64AsNumber<T, K extends keyof T> = Omit<T, K> & { [P in K]: number }
