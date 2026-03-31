# If GitHub Actions Do Not Work - First Check This!

If your GitHub Actions workflows are failing to start, staying queued indefinitely, or failing with errors related to quotas, the very first thing you should check is your **Spending Limit**.

## Check Your Spending Limit

1.  Go to your GitHub Billing Settings:
    **[https://github.com/settings/billing](https://github.com/settings/billing)**

2.  Look for the **"Spending limit"** section.

3.  Ensure you have not set a limit of `$0` if you have used up your free included minutes (2,000 minutes/month for free accounts). 

4.  If your limit is reached or set to $0 and you have no free minutes left, GitHub will **pause** all your Actions workflows.

### Why does this happen?
GitHub Actions provides a generous free tier, but running builds on macOS and Windows runners consumes these minutes much faster than Linux runners (macOS runners cost 10x more minutes!). If you run out of minutes, your builds will simply stop working until you either:
*   Wait for the next billing cycle (month) to reset.
*   Increase your spending limit to allow for a small charge (pay-as-you-go).
