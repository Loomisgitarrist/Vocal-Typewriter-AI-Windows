# 🍎 Complete Mac App Store Submission Guide
**Team ID:** `555HHF3KGU`
**Developer:** Loomis Connell Green

You currently have a "Direct Download" certificate. For the App Store, you need **two new certificates**.

---

## 🛑 STEP 1: Create the Certificates (Do this now)

You cannot build for the App Store without these.

### A. Generate a CSR (Certificate Signing Request) on your Mac
1.  Open **Keychain Access** (Cmd+Space, type "Keychain Access").
2.  In the top menu bar, click **Keychain Access** > **Certificate Assistant** > **Request a Certificate From a Certificate Authority**.
3.  **User Email Address:** (Your Apple ID email)
4.  **Common Name:** Loomis Green
5.  **CA Email Address:** (Leave empty)
6.  **Request is:** Select "Saved to disk".
7.  Click **Continue** and save `CertificateSigningRequest.certSigningRequest` to your Desktop.

### B. Create Certificates on Apple Developer Website
1.  Go to [developer.apple.com/account/resources/certificates/add](https://developer.apple.com/account/resources/certificates/add).
2.  **Certificate #1 (For the App):**
    *   Select **"Mac App Distribution"** (under Software).
    *   Click **Continue**.
    *   Upload the `CertificateSigningRequest` file from your Desktop.
    *   **Download** the resulting `.cer` file (e.g., `mac_app.cer`).
3.  **Certificate #2 (For the Installer):**
    *   Go back to "Add Certificate".
    *   Select **"Mac Installer Distribution"** (under Software).
    *   Click **Continue**.
    *   Upload the *same* `CertificateSigningRequest` file.
    *   **Download** this `.cer` file too (e.g., `mac_installer.cer`).

### C. Install Them (CRITICAL STEP)
1.  Find the two downloaded `.cer` files in your Downloads folder (`mac_app.cer` and `mac_installer.cer`).
2.  **Double-click** each one.
3.  When prompted by Keychain Access, ensure the "Keychain" dropdown says **"login"** (NOT "System").
4.  Click **Add**.
5.  They are now installed.

---

## 🆔 STEP 2: Create App ID & Profile

1.  **Register App ID:**
    *   Go to [Identifiers](https://developer.apple.com/account/resources/identifiers/list).
    *   Click **(+)**.
    *   Select **App IDs** > **App**.
    *   **Description:** Vocal Typewriter
    *   **Bundle ID:** `com.vocaltypewriter.app`
    *   **Capabilities:** Check "App Sandbox" and "Network Sockets" (Client & Server).
    *   Click **Register**.

2.  **Create Provisioning Profile:**
    *   Go to [Profiles](https://developer.apple.com/account/resources/profiles/list).
    *   Click **(+)**.
    *   Select **"Mac App Store"** (under Distribution).
    *   Select the App ID you just created (`com.vocaltypewriter.app`).
    *   Select your **"Mac App Distribution"** certificate (Loomis Green).
    *   **Profile Name:** `Vocal Typewriter App Store`.
    *   **Download** the `.provisionprofile` file.
    *   **Double-click** it to install.

---

## 🏗️ STEP 3: Build & Package

Once Step 1 & 2 are done, run these commands in your terminal:

```bash
# 1. Build the App (Sandboxed)
./build-appstore-macos.sh

# 2. Sign and Package for Upload
# Note: "3rd Party Mac Developer Installer: Loomis Connell Green (555HHF3KGU)" is the name of Certificate #2 you installed.

productbuild --component "src-tauri/target/release/bundle/macos/Vocal Typewriter.app" /Applications --sign "3rd Party Mac Developer Installer: Loomis Connell Green (555HHF3KGU)" --product "src-tauri/Info.plist" "VocalTypewriter.pkg"
```

---

## 🚀 STEP 4: Upload

1.  Open **Transporter** (App from Mac App Store).
2.  Sign in with your Apple ID.
3.  Click **(+)** and select the `VocalTypewriter.pkg` file you just created.
4.  Click **Deliver**.

---

## 📝 STEP 5: App Store Connect

1.  Go to [App Store Connect](https://appstoreconnect.apple.com).
2.  Create the app entry if you haven't (My Apps > +).
3.  Fill in screenshots, description, price ($59.99), and privacy policy.
4.  Select the build you uploaded via Transporter.
5.  **Submit for Review**.
