# tts 0.26.3 finds this class and backendId through JNI_OnLoad/GetFieldID.
-keep class rs.tts.Bridge { *; }
-keep class org.deflorta.android.DeflortaActivity { *; }
# GameActivity's native glue looks up methods and GameTextInput fields by name.
-keep class com.google.androidgamesdk.** { *; }
