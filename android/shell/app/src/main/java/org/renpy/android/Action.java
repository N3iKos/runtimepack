package org.renpy.android;

import android.content.Context;
import android.content.Intent;
import android.net.Uri;

public class Action {
    static Context context;

    static void send(String mimeType, String filename, String subject, String text, String chooser_title) {
        if (context == null) return;
        Intent emailIntent = new Intent(Intent.ACTION_SEND);
        emailIntent.setType(mimeType);
        if (subject != null)
            emailIntent.putExtra(Intent.EXTRA_SUBJECT, subject);
        if (text != null)
            emailIntent.putExtra(Intent.EXTRA_TEXT, text);
        if (filename != null)
            emailIntent.putExtra(Intent.EXTRA_STREAM, Uri.parse("file://" + filename));
        if (chooser_title == null)
            chooser_title = "Send";
        context.startActivity(Intent.createChooser(emailIntent, chooser_title));
    }
}
