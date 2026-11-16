package android.content.res;

import java.io.ByteArrayInputStream;
import java.io.FileNotFoundException;
import java.io.IOException;
import java.io.InputStream;
import java.util.HashMap;
import java.util.HashSet;
import java.util.Map;
import java.util.Set;

/**
 * Test-only boundary double for Android AssetManager.
 * Used exclusively by unit tests to provide loose APK asset byte streams
 * without requiring the full Android framework or device runtime.
 */
public class AssetManager implements AutoCloseable {
    private final Map<String, byte[]> assets = new HashMap<>();

    public AssetManager() {}

    public void addAsset(String path, byte[] content) {
        String clean = path.replace('\\', '/').replaceAll("^/+", "");
        assets.put(clean, content);
    }

    public InputStream open(String fileName) throws IOException {
        String clean = fileName.replace('\\', '/').replaceAll("^/+", "");
        byte[] bytes = assets.get(clean);
        if (bytes == null) {
            throw new FileNotFoundException("Asset not found: " + fileName);
        }
        return new ByteArrayInputStream(bytes);
    }

    public AssetFileDescriptor openFd(String fileName) throws IOException {
        throw new FileNotFoundException("openFd stub for test: " + fileName);
    }

    public String[] list(String path) throws IOException {
        String clean = path.replace('\\', '/').replaceAll("^/+", "").replaceAll("/+$", "");
        String prefix = clean.isEmpty() ? "" : clean + "/";
        Set<String> result = new HashSet<>();
        for (String k : assets.keySet()) {
            if (prefix.isEmpty() || k.startsWith(prefix)) {
                String sub = k.substring(prefix.length());
                int slash = sub.indexOf('/');
                if (slash != -1) {
                    result.add(sub.substring(0, slash));
                } else if (!sub.isEmpty()) {
                    result.add(sub);
                }
            }
        }
        return result.toArray(new String[0]);
    }

    @Override
    public void close() {}
}
