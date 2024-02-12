; ModuleID = 'probe5.c85dd862f7b9a3d0-cgu.0'
source_filename = "probe5.c85dd862f7b9a3d0-cgu.0"
target datalayout = "e-m:e-p:32:32-Fi8-i64:64-v128:64:128-a:0:32-n32-S64"
target triple = "thumbv6m-none-unknown-eabi"

@alloc_070f122b6501f82f36a0b0b6f135e030 = private unnamed_addr constant <{ [75 x i8] }> <{ [75 x i8] c"/rustc/07dca489ac2d933c78d3c5158e3f43beefeb02ce/library/core/src/num/mod.rs" }>, align 1
@alloc_d6df43990613b53f983c7b3765b94769 = private unnamed_addr constant <{ ptr, [12 x i8] }> <{ ptr @alloc_070f122b6501f82f36a0b0b6f135e030, [12 x i8] c"K\00\00\00y\04\00\00\05\00\00\00" }>, align 4
@str.0 = internal unnamed_addr constant [25 x i8] c"attempt to divide by zero"

; probe5::probe
; Function Attrs: nounwind
define dso_local void @_ZN6probe55probe17h6b735cd786f2cfc0E() unnamed_addr #0 {
start:
  %0 = call i1 @llvm.expect.i1(i1 false, i1 false)
  br i1 %0, label %panic.i, label %"_ZN4core3num21_$LT$impl$u20$u32$GT$10div_euclid17h249863b0a8dafe39E.exit"

panic.i:                                          ; preds = %start
; call core::panicking::panic
  call void @_ZN4core9panicking5panic17h0408a03213fe3e68E(ptr align 1 @str.0, i32 25, ptr align 4 @alloc_d6df43990613b53f983c7b3765b94769) #3
  unreachable

"_ZN4core3num21_$LT$impl$u20$u32$GT$10div_euclid17h249863b0a8dafe39E.exit": ; preds = %start
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(none)
declare i1 @llvm.expect.i1(i1, i1) #1

; core::panicking::panic
; Function Attrs: cold noinline noreturn nounwind
declare dso_local void @_ZN4core9panicking5panic17h0408a03213fe3e68E(ptr align 1, i32, ptr align 4) unnamed_addr #2

attributes #0 = { nounwind "frame-pointer"="all" "target-cpu"="generic" "target-features"="+strict-align,+atomics-32" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(none) }
attributes #2 = { cold noinline noreturn nounwind "frame-pointer"="all" "target-cpu"="generic" "target-features"="+strict-align,+atomics-32" }
attributes #3 = { noreturn nounwind }

!llvm.ident = !{!0}

!0 = !{!"rustc version 1.76.0 (07dca489a 2024-02-04)"}
