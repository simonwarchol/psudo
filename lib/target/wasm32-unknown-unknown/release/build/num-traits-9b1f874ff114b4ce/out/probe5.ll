; ModuleID = 'probe5.e1614ca6-cgu.0'
source_filename = "probe5.e1614ca6-cgu.0"
target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"

@alloc3 = private unnamed_addr constant <{ [75 x i8] }> <{ [75 x i8] c"/rustc/9eb3afe9ebe9c7d2b84b71002d44f4a0edac95e0/library/core/src/num/mod.rs" }>, align 1
@alloc4 = private unnamed_addr constant <{ ptr, [12 x i8] }> <{ ptr @alloc3, [12 x i8] c"K\00\00\00\99\03\00\00\05\00\00\00" }>, align 4
@str.0 = internal constant [25 x i8] c"attempt to divide by zero"

; probe5::probe
; Function Attrs: nounwind
define hidden void @_ZN6probe55probe17h79253f28a7417087E() unnamed_addr #0 {
start:
  %0 = call i1 @llvm.expect.i1(i1 false, i1 false)
  br i1 %0, label %panic.i, label %"_ZN4core3num21_$LT$impl$u20$u32$GT$10div_euclid17hdf35a2dbda19cd84E.exit"

panic.i:                                          ; preds = %start
; call core::panicking::panic
  call void @_ZN4core9panicking5panic17h54beb3f515dd3c6dE(ptr align 1 @str.0, i32 25, ptr align 4 @alloc4) #3
  unreachable

"_ZN4core3num21_$LT$impl$u20$u32$GT$10div_euclid17hdf35a2dbda19cd84E.exit": ; preds = %start
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind readnone willreturn
declare hidden i1 @llvm.expect.i1(i1, i1) #1

; core::panicking::panic
; Function Attrs: cold noinline noreturn nounwind
declare dso_local void @_ZN4core9panicking5panic17h54beb3f515dd3c6dE(ptr align 1, i32, ptr align 4) unnamed_addr #2

attributes #0 = { nounwind "target-cpu"="generic" }
attributes #1 = { nocallback nofree nosync nounwind readnone willreturn }
attributes #2 = { cold noinline noreturn nounwind "target-cpu"="generic" }
attributes #3 = { noreturn nounwind }
