# Important Notes Before Signing & Releasing

## Windows Signing & SmartScreen Warnings

**Crucial for User Experience:**
Even though we are now "signing" the Windows application with the self-signed certificate we generated, **users will still likely see a security warning.**

### The Warning
When a user opens the `.exe` installer, they will see a blue screen saying:
> **"Windows protected your PC"**
> Microsoft Defender SmartScreen prevented an unrecognized app from starting.

### How Users Install It
You must instruct your users to:
1.  Click the **"More info"** text link on that blue screen.
2.  Click the **"Run anyway"** button that appears.

### Why is this happening?
We are using a **Self-Signed Certificate**. This proves the app integrity (it hasn't been tampered with), but it does **not** prove your identity to Microsoft. To remove this warning completely, you would need to purchase a commercial "EV Code Signing Certificate" (approx. $300-$500/year) from a vendor like Sectigo or DigiCert.

---

## macOS Support (Universal Binary)

We have updated the build process to support a wider range of Mac devices.

### 1. Intel & Apple Silicon Support
The build now produces a **Universal Binary**.
*   **Previously:** It only worked on the specific chip type of the build machine (likely just Apple Silicon).
*   **Now:** The single `.dmg` file works natively on **both**:
    *   Old Macs (Intel chips)
    *   New Macs (M1, M2, M3, M4 chips)

### 2. Minimum macOS Version
We have lowered the minimum requirement:
*   **New Minimum:** macOS 10.15 (Catalina)
*   **Previous:** macOS 12.0 (Monterey)

This ensures users with Macs from ~2012-2013 onwards can generally run the application, assuming they are on at least Catalina.
