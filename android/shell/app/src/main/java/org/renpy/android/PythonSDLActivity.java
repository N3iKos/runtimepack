package org.renpy.android;

import android.content.Context;
import android.content.Intent;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.os.PowerManager;
import android.os.VibrationEffect;
import android.os.Vibrator;
import android.util.DisplayMetrics;
import android.util.Log;
import android.view.View;
import android.view.ViewGroup;
import android.widget.FrameLayout;
import android.widget.ImageView;
import android.widget.LinearLayout;
import android.widget.Toast;

import org.libsdl.app.SDLActivity;

public class PythonSDLActivity extends SDLActivity {

    private static final String TAG = "PythonSDLActivity";

    /**
     * This exists so Python code can access this activity via PyJNIus:
     * autoclass("org.renpy.android.PythonSDLActivity").mActivity
     */
    public static PythonSDLActivity mActivity = null;

    public FrameLayout mFrameLayout = null;
    public LinearLayout mVbox = null;
    public ImageView mPresplash = null;
    public PowerManager.WakeLock wakeLock = null;
    public StoreInterface mStore = null;
    public ResourceManager resourceManager = null;

    public int mActivityResultRequestCode = -1;
    public int mActivityResultResultCode = -1;
    public Intent mActivityResultResultData = null;

    @Override
    protected String[] getLibraries() {
        return new String[] {
            "renpython",
        };
    }

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        mActivity = this;
        super.onCreate(savedInstanceState);
    }

    @Override
    public void setContentView(View view) {
        mFrameLayout = new FrameLayout(this);
        mFrameLayout.addView(view);

        mVbox = new LinearLayout(this);
        mVbox.setOrientation(LinearLayout.VERTICAL);
        mVbox.addView(mFrameLayout, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, 0, 1.0f));

        super.setContentView(mVbox);
    }

    public void addView(View view, int index) {
        if (mVbox != null) {
            mVbox.addView(view, index, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT,
                0.0f));
        }
    }

    public void removeView(View view) {
        if (mVbox != null) {
            mVbox.removeView(view);
        }
    }

    public void createStore() {
        if ("none".equals(Constants.store)) {
            return;
        }
        try {
            Class<?> cls = Class.forName("org.renpy.iap.Store");
            cls.getMethod("create", PythonSDLActivity.class).invoke(null, this);
        } catch (Exception e) {
            Log.e(TAG, "Failed to create store: " + e.getMessage());
        }
    }

    // Public APIs called by Ren'Py / Python (e.g. lib/python3.9/android/__init__.py)

    public void openUrl(String url) {
        try {
            Intent intent = new Intent(Intent.ACTION_VIEW, Uri.parse(url));
            intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
            startActivity(intent);
        } catch (Exception e) {
            Log.w(TAG, "Failed to open URL: " + url, e);
        }
    }

    public void vibrate(double s) {
        try {
            Vibrator v = (Vibrator) getSystemService(Context.VIBRATOR_SERVICE);
            if (v != null && v.hasVibrator()) {
                long ms = (long) (1000.0 * s);
                if (ms > 0) {
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                        v.vibrate(VibrationEffect.createOneShot(ms, VibrationEffect.DEFAULT_AMPLITUDE));
                    } else {
                        v.vibrate(ms);
                    }
                }
            }
        } catch (Exception e) {
            Log.w(TAG, "Vibrate failed: " + e.getMessage());
        }
    }

    public int getDPI() {
        try {
            DisplayMetrics metrics = new DisplayMetrics();
            getWindowManager().getDefaultDisplay().getMetrics(metrics);
            return metrics.densityDpi;
        } catch (Exception e) {
            return getResources().getDisplayMetrics().densityDpi;
        }
    }

    public void armOnStop() {
        Log.d(TAG, "armOnStop called");
    }

    public void finishOnStop() {
        Log.d(TAG, "finishOnStop called");
    }

    public void setWakeLock(final boolean active) {
        runOnUiThread(new Runnable() {
            @Override
            public void run() {
                try {
                    if (active) {
                        getWindow().addFlags(android.view.WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
                    } else {
                        getWindow().clearFlags(android.view.WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
                    }
                } catch (Exception e) {
                    Log.w(TAG, "setWakeLock failed: " + e.getMessage());
                }
            }
        });
    }

    public void toast(final String msg) {
        runOnUiThread(new Runnable() {
            @Override
            public void run() {
                Toast.makeText(PythonSDLActivity.this, msg, Toast.LENGTH_SHORT).show();
            }
        });
    }

    public void toastError(final String msg) {
        runOnUiThread(new Runnable() {
            @Override
            public void run() {
                Toast.makeText(PythonSDLActivity.this, msg, Toast.LENGTH_LONG).show();
            }
        });
    }

    public void hidePresplash() {
        runOnUiThread(new Runnable() {
            @Override
            public void run() {
                if (mPresplash != null && mLayout != null) {
                    mLayout.removeView(mPresplash);
                    mPresplash = null;
                }
            }
        });
    }

    public static boolean isChromebook() {
        return SDLActivity.isChromebook();
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent resultData) {
        if (mStore != null && mStore.onActivityResult(requestCode, resultCode, resultData)) {
            return;
        }
        mActivityResultRequestCode = requestCode;
        mActivityResultResultCode = resultCode;
        mActivityResultResultData = resultData;
        super.onActivityResult(requestCode, resultCode, resultData);
    }

    @Override
    protected void onDestroy() {
        if (wakeLock != null && wakeLock.isHeld()) {
            try {
                wakeLock.release();
            } catch (Exception ignored) {}
        }
        if (mStore != null) {
            try {
                mStore.destroy();
            } catch (Exception ignored) {}
            mStore = null;
        }
        if (mActivity == this) {
            mActivity = null;
        }
        super.onDestroy();
    }

    public native void nativeSetEnv(String variable, String value);
}
