# JNA ve UniFFI yansıma (reflection) ile çalışır: sınıfları koru.
-keep class com.sun.jna.** { *; }
-keepclassmembers class * extends com.sun.jna.** { public *; }
-keep class uniffi.** { *; }
-dontwarn java.awt.**
-dontwarn com.sun.jna.**
