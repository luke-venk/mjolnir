import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "Mjölnir",
  description: "2 cameras and a crazy dream to revolutionize B2B AI Saas...",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body className="antialiased">
        {children}
      </body>
    </html>
  );
}
