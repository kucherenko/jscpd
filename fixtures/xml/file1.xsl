<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
    <xsl:output
            encoding="UTF-8"
            method="html"
            omit-xml-declaration="yes"
            indent="no"
            doctype-public="-//W3C//DTD XHTML 1.0 Strict//EN"
            doctype-system="http://www.w3.org/TR/xhtml1/DTD/xhtml1-strict.dtd"/>

    <xsl:template match="pmd-cpd">
        <html xmlns="http://www.w3.org/1999/xhtml">
            <head>
                <meta http-equiv="content-type" content="text/html; charset=utf-8"/>
                <title>Copy-Paste Detection Report</title>
            </head>
            <body>
                <xsl:apply-templates select="duplication" />
            </body>
        </html>
    </xsl:template>

    <xsl:template match="duplication">
      <div>
        <xsl:apply-templates select="file"/>
        <div><pre><xsl:value-of select="codefragment"/></pre></div>
      </div>
      <hr/>
    </xsl:template>

    <xsl:template match="file">
      <div><strong>File <xsl:value-of select="position()"/>:</strong> <xsl:value-of select="@path"/>:<xsl:value-of select="@line"/></div>
    </xsl:template>

    <xsl:template match="duplication">
      <div>
        <xsl:apply-templates select="file"/>
        <div><pre><xsl:value-of select="codefragment"/></pre></div>
      </div>
      <hr/>
    </xsl:template>

    <xsl:template match="file">
      <div><strong>File <xsl:value-of select="position()"/>:</strong> <xsl:value-of select="@path"/>:<xsl:value-of select="@line"/></div>
    </xsl:template>

</xsl:stylesheet>
