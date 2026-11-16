import glob
import xml.etree.ElementTree as ET

total = 0
failures = 0
errors = 0
for f in glob.glob("android/shell/app/build/test-results/testDebugUnitTest/*.xml"):
    tree = ET.parse(f)
    root = tree.getroot()
    total += int(root.attrib.get("tests", 0))
    failures += int(root.attrib.get("failures", 0))
    errors += int(root.attrib.get("errors", 0))
print(f"Total JVM tests: {total}, Failures: {failures}, Errors: {errors}")
