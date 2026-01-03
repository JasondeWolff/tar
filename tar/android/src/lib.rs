use android_activity::AndroidApp;

#[no_mangle]
fn android_main(app: AndroidApp) {
    tar::internal_main(app);
}
