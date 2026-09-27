package org.renpy.android;

import android.content.Intent;

public interface StoreInterface {
    void destroy();
    boolean onActivityResult(int requestCode, int resultCode, Intent intent);
}
