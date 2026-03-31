# Windows Code Signing for GitHub Actions

This guide explains how to generate a self-signed certificate for signing Windows executables in GitHub Actions, and how to configure the necessary secrets.

## 1. Generate a Self-Signed Certificate (Mac/Linux)

If you do not have a purchased certificate (e.g., from Sectigo or DigiCert), you can generate a self-signed one using `openssl`.

### A. Generate Private Key and Certificate
Run the following command in your terminal. You can adjust the Subject (`-subj`) as needed.

```bash
openssl req -x509 -newkey rsa:4096 \
  -keyout temp_private.key \
  -out temp_cert.crt \
  -days 3650 \
  -nodes \
  -subj "/C=US/O=SpeakToMeLG/CN=SpeakToMeLG"
```

### B. Export to PFX Format
Combine the key and certificate into a `.pfx` file (PKCS#12 format), which Windows uses for signing.
**Important:** You will be prompted to set a password. Remember this password!

```bash
openssl pkcs12 -export \
  -out windows_sign_cert.pfx \
  -inkey temp_private.key \
  -in temp_cert.crt \
  -passout pass:YOUR_SECURE_PASSWORD
```
*(Replace `YOUR_SECURE_PASSWORD` with your actual password)*

### C. Clean Up
Remove the temporary key and certificate files:

```bash
rm temp_private.key temp_cert.crt
```

## 2. Prepare for GitHub Secrets

GitHub Actions cannot store binary files directly. We must convert the `.pfx` file into a Base64 encoded string.

Run this command:

```bash
base64 -i windows_sign_cert.pfx | tr -d '\n'
```
*(The `tr -d '\n'` part ensures the output is a single long line, which prevents formatting issues in GitHub Secrets).*

## 3. Configure GitHub Secrets

1.  Go to your GitHub Repository.
2.  Navigate to **Settings** -> **Secrets and variables** -> **Actions**.
3.  Click **New repository secret** and add the following two secrets:

| Secret Name | Value |
| :--- | :--- |
| `WINDOWS_CERT_PASSWORD` | The password you set in Step 1B. |
| `WINDOWS_CERT_PFX` | The long Base64 string generated in Step 2. |

## 4. Build System & Error Handling (Latest Info)

**Update (Latest):** The GitHub Actions workflow (`build.yml`) now includes a **Robust Fallback System**.

### How it works:
1.  **Auto-Verification:** The build script attempts to decode your `WINDOWS_CERT_PFX` secret and open it with `WINDOWS_CERT_PASSWORD`.
2.  **Double-Base64 Fix:** It automatically detects if the secret was accidentally Base64-encoded twice (a common error) and fixes it.
3.  **Automatic Fallback:** If the secret is missing, corrupt, invalid, or has the wrong password, the build **will not fail**.
    *   Instead, it automatically generates a temporary, valid self-signed certificate on the fly.
    *   The app will be signed with this temporary certificate.
    *   This ensures you always get a working `.exe` installer at the end of the build, even if the signing secrets are messed up.

### Implication for Users:
Regardless of whether the "Real" cert or "Fallback" cert is used, users will see the standard "Windows protected your PC" (SmartScreen) warning unless you purchase an EV Certificate. They simply need to click **"More info" -> "Run anyway"**.

## Using a Purchased Certificate
If you buy a code signing certificate later (to avoid "Unknown Publisher" warnings):
1.  You will receive a `.pfx` file from the vendor.
2.  Skip Step 1.
3.  Follow Step 2 to get the Base64 string of your purchased `.pfx`.
4.  Update the GitHub Secrets with the new Base64 string and the password provided by the vendor.
